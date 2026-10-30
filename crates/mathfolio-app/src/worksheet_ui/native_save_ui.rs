//! Session save identity and dialog-free save dispatch.
//! Loaded native source remains authoritative; successful Save does not rebase it.
use super::*;
use mathfolio_document::native_save::DiskRevision;

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(super) struct NativeTarget {
    path: PathBuf,
    revision: DiskRevision,
}
#[cfg(not(target_arch = "wasm32"))]
impl NativeTarget {
    pub(super) fn from_open(path: &Path, document: &CanvasDocument) -> Result<Self, String> {
        let archive = document
            .native_archive
            .as_deref()
            .ok_or("Native import did not retain source bytes")?;
        Ok(Self {
            path: path.to_owned(),
            revision: DiskRevision::from_bytes(archive)?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(not(target_arch = "wasm32"))]
pub(super) enum SaveIntent {
    Current,
    SaveAs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(not(target_arch = "wasm32"))]
enum Format {
    Own,
    Native,
}
#[cfg(not(target_arch = "wasm32"))]
impl Format {
    fn from_path(path: &Path) -> Result<Self, String> {
        match path.extension().and_then(|value| value.to_str()) {
            Some(value) if mathfolio_document::is_own_extension(value) => Ok(Self::Own),
            Some(value) if value.eq_ignore_ascii_case("mcdx") => Ok(Self::Native),
            _ => Err("Save requires an explicit .mathfolio or .mcdx extension".into()),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
enum Destination {
    Own(PathBuf),
    NewNative(PathBuf),
    ReplaceNative(PathBuf, DiskRevision),
}

/// UI-only choice. Capture revision after this returns, before editor/export preparation.
/// The current format/name is preferred; only the selected known extension dispatches.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn choose_destination(current: Option<&Path>) -> Result<Option<PathBuf>, String> {
    let format = current
        .map(Format::from_path)
        .transpose()?
        .unwrap_or(Format::Own);
    let dialog = rfd::FileDialog::new();
    let dialog = match format {
        Format::Own => dialog
            .add_filter("Mathfolio worksheet", &["mathfolio", "mathcraft"])
            .add_filter("Mathcad worksheet", &["mcdx"]),
        Format::Native => dialog
            .add_filter("Mathcad worksheet", &["mcdx"])
            .add_filter("Mathfolio worksheet", &["mathfolio", "mathcraft"]),
    };
    let default = match format {
        Format::Own => "worksheet.mathfolio",
        Format::Native => "worksheet.mcdx",
    };
    let directory = current
        .and_then(Path::parent)
        .filter(|path| !path.as_os_str().is_empty())
        .map(Path::to_owned)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let name = current
        .and_then(Path::file_name)
        .unwrap_or_else(|| default.as_ref());
    Ok(dialog
        .set_directory(directory)
        .set_file_name(name.to_string_lossy())
        .save_file())
}

/// Injectable UI boundary. Scripted implementations never display a dialog.
/// A decline/closed question and a cancelled chooser both return before source,
/// editor, composition, cache, date, history or save identity preparation.
#[cfg(not(target_arch = "wasm32"))]
pub(super) trait SavePrompts {
    fn readonly_save_as(&mut self, path: &Path) -> bool;
    fn destination(&mut self, current: Option<&Path>) -> Result<Option<PathBuf>, String>;
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) struct SystemSavePrompts;
#[cfg(not(target_arch = "wasm32"))]
impl SavePrompts for SystemSavePrompts {
    fn readonly_save_as(&mut self, path: &Path) -> bool {
        rfd::MessageDialog::new()
            .set_title("Mathfolio")
            .set_description(format!(
                "{} 파일은 읽기 전용입니다. 다른 이름으로 저장하시겠습니까?",
                path.display()
            ))
            .set_level(rfd::MessageLevel::Warning)
            .set_buttons(rfd::MessageButtons::YesNo)
            .show()
            == rfd::MessageDialogResult::Yes
    }
    fn destination(&mut self, current: Option<&Path>) -> Result<Option<PathBuf>, String> {
        choose_destination(current)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl WorksheetApp {
    pub(super) fn save_with_prompts(
        &mut self,
        save_as: bool,
        prompts: &mut impl SavePrompts,
    ) -> bool {
        let mut choose = save_as || self.path.is_none();
        if !choose
            && let Some(path) = self.path.as_deref()
            && self
                .native_save_target
                .as_ref()
                .is_some_and(|target| target.path.as_path() == path)
            && matches!(Format::from_path(path), Ok(Format::Native))
            && std::fs::symlink_metadata(path)
                .ok()
                .is_some_and(|metadata| {
                    metadata.file_type().is_file() && metadata.permissions().readonly()
                })
        {
            // Narrow attribute-positive preflight only. Missing/unknown paths,
            // ACL/lock/reparse/write failures still use the unchanged writer.
            // Do not write-open a user file, classify error strings or retry.
            if !prompts.readonly_save_as(path) {
                return false;
            }
            choose = true;
        }
        let path = if choose {
            match prompts.destination(self.path.as_deref()) {
                Ok(path) => path,
                Err(error) => {
                    self.status = format!("Could not save: {error}");
                    return false;
                }
            }
        } else {
            self.path.clone()
        };
        self.save_to_destination(
            path,
            if choose {
                SaveIntent::SaveAs
            } else {
                SaveIntent::Current
            },
        )
    }

    fn capture_save_destination(
        &self,
        path: PathBuf,
        intent: SaveIntent,
    ) -> Result<Destination, String> {
        match Format::from_path(&path)? {
            // Explicit own paths remain compatible with existing fixtures and
            // documents that embed a native archive. The archive does not select format.
            Format::Own => Ok(Destination::Own(path)),
            Format::Native if intent == SaveIntent::Current => {
                let target = self
                    .native_save_target
                    .as_ref()
                    .filter(|target| target.path == path)
                    .ok_or("A different native destination requires Save As")?;
                Ok(Destination::ReplaceNative(path, target.revision))
            }
            Format::Native => {
                // This explicit choice permits overwriting only the bytes
                // observed now. Do not capture after expensive preparation.
                match std::fs::symlink_metadata(&path) {
                    Ok(_) => Ok(Destination::ReplaceNative(
                        path.clone(),
                        DiskRevision::read(&path)?,
                    )),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        Ok(Destination::NewNative(path))
                    }
                    Err(error) => Err(error.to_string()),
                }
            }
        }
    }

    /// No dialogs. None is a cancellation and changes no source/history/identity.
    /// Actual I/O failure retains accepted edits and their existing dirty/history state.
    pub(super) fn save_to_destination(
        &mut self,
        path: Option<PathBuf>,
        intent: SaveIntent,
    ) -> bool {
        let Some(path) = path else {
            return false;
        };
        let destination = match self.capture_save_destination(path, intent) {
            Ok(destination) => destination,
            Err(error) => {
                self.status = format!("Could not save: {error}");
                return false;
            }
        };
        self.focus_new = self.editing_region.or(self.focus_new);
        if let Err(error) = self.sync_editors() {
            self.status = format!("Could not save: {error}");
            return false;
        }
        self.commit_math_edits();
        self.persist_calculation_cache();

        let result: Result<(PathBuf, Option<NativeTarget>), String> = (|| match destination {
            Destination::Own(path) => {
                self.document.save(&path)?;
                Ok((path, None))
            }
            Destination::NewNative(path) => {
                let prepared = mathfolio_document::export::prepare_mcdx(&self.document)?;
                let revision = prepared.write_new(&path)?;
                Ok((path.clone(), Some(NativeTarget { path, revision })))
            }
            Destination::ReplaceNative(path, expected) => {
                let prepared = mathfolio_document::export::prepare_mcdx(&self.document)?;
                let revision = prepared.replace_expected(&path, &expected)?;
                Ok((path.clone(), Some(NativeTarget { path, revision })))
            }
        })();
        match result {
            Ok((path, native_target)) => {
                // Revision comes from the successful prepared write. A later
                // metadata lookup cannot turn a completed save into failure.
                self.saved_date =
                    crate::page_fields::SavedDate::from_path(&path).unwrap_or_default();
                self.path = Some(path.clone());
                self.native_save_target = native_target;
                self.dirty = false;
                self.status = format!("Saved {}", path.display());
                // Deliberately no document.native_archive replacement, import,
                // editor/history reset, region-ID remap or cache promotion.
                true
            }
            Err(error) => {
                self.status = format!("Could not save: {error}");
                false
            }
        }
    }
}

#[cfg(test)]
#[path = "native_save_ui_tests.rs"]
mod tests;
