//! Explicit native save intent, independent of import authority.
//! Portable revision check + sibling-temp replace is NOT an atomic compare/swap
//! against concurrent writers; check-to-rename races remain possible.
use crate::native::NativePackage;
use sha2::{Digest, Sha256};
#[cfg(not(target_arch = "wasm32"))]
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use std::{
    io::{Cursor, Seek, SeekFrom, Write},
    sync::Arc,
};

pub const MAX_NATIVE_SAVE_BYTES: usize = 16 * 1024 * 1024;
type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiskRevision {
    length: u64,
    sha256: [u8; 32],
}
impl DiskRevision {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_NATIVE_SAVE_BYTES {
            return Err("Native file exceeds16MiB session read limit".into());
        }
        Ok(Self {
            length: bytes.len() as u64,
            sha256: Sha256::digest(bytes).into(),
        })
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn read(path: &Path) -> Result<Self> {
        read_revision(path, false)
    }
    pub fn len(&self) -> u64 {
        self.length
    }
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }
    pub fn sha256(&self) -> [u8; 32] {
        self.sha256
    }
}

/// Constructor stays inside crate/export. Bytes are produced only through the
/// full existing exporter and bounded native ZIP writer, not arbitrary input.
#[derive(Clone, Debug)]
pub struct PreparedNativeSave {
    bytes: Arc<[u8]>,
    revision: DiskRevision,
}
impl PreparedNativeSave {
    pub(crate) fn from_package(package: NativePackage) -> Result<Self> {
        let bytes = package.to_bytes_bounded(MAX_NATIVE_SAVE_BYTES)?;
        let revision = DiskRevision::from_bytes(&bytes)?;
        Ok(Self {
            bytes: bytes.into(),
            revision,
        })
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn revision(&self) -> DiskRevision {
        self.revision
    }

    /// Save As fresh output only; existing export_mcdx remains unchanged.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn write_new(&self, path: &Path) -> Result<DiskRevision> {
        extension(path)?;
        let path = checked_path(path, true)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        let result = file.write_all(&self.bytes).and_then(|_| file.sync_all());
        drop(file);
        if let Err(error) = result {
            remove_owned_file(&path);
            return Err(error.to_string());
        }
        Ok(self.revision)
    }

    /// Existing target only, explicit expected old content revision required.
    /// Success returns prepared revision without a fallible post-write read.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn replace_expected(&self, path: &Path, expected: &DiskRevision) -> Result<DiskRevision> {
        self.replace_with_hook(path, expected, || {})
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn replace_with_hook(
        &self,
        path: &Path,
        expected: &DiskRevision,
        before_recheck: impl FnOnce(),
    ) -> Result<DiskRevision> {
        extension(path)?;
        let path = checked_path(path, false)?;
        if read_revision(&path, true)? != *expected {
            return Err("Native target changed externally".into());
        }
        let permissions = fs::metadata(&path)
            .map_err(|e| e.to_string())?
            .permissions();
        let mut temp = SiblingTemp::create(&path)?;
        let file = temp.file.as_mut().ok_or("Temporary file unavailable")?;
        file.write_all(&self.bytes).map_err(|e| e.to_string())?;
        // Apply after writing: on Unix a write can clear special mode bits.
        // Until then the new Unix file keeps its restrictive creation mode.
        file.set_permissions(permissions)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(temp.file.take());
        before_recheck();
        // Recheck after potentially slow ZIP writing/sync. This narrows, but
        // cannot eliminate, check->rename races without an OS-level CAS.
        if read_revision(&path, true)? != *expected {
            return Err("Native target changed while preparing save".into());
        }
        fs::rename(&temp.path, &path).map_err(|e| e.to_string())?;
        temp.committed = true;
        Ok(self.revision)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn extension(path: &Path) -> Result<()> {
    if path
        .extension()
        .and_then(|s| s.to_str())
        .is_none_or(|s| !s.eq_ignore_ascii_case("mcdx"))
    {
        return Err("Native save requires .mcdx".into());
    }
    Ok(())
}
#[cfg(not(target_arch = "wasm32"))]
fn unsupported_link(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}
/// Reject known symlink/reparse entries, including parent directories. Does
/// not claim protection against a hostile directory being swapped afterward.
#[cfg(not(target_arch = "wasm32"))]
fn checked_path(path: &Path, allow_missing_final: bool) -> Result<PathBuf> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(path)
    };
    if path.file_name().is_none() {
        return Err("Native target filename missing".into());
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        if component == Component::ParentDir {
            return Err("Parent traversal unsupported for native replacement".into());
        }
        current.push(component.as_os_str());
        if matches!(component, Component::Prefix(_) | Component::CurDir) {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if unsupported_link(&metadata) {
                    return Err("Native save symlink/reparse path unsupported".into());
                }
                if current != path && !metadata.is_dir() {
                    return Err("Native parent is not a directory".into());
                }
            }
            Err(error)
                if allow_missing_final
                    && current == path
                    && error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(path)
}
#[cfg(not(target_arch = "wasm32"))]
fn read_revision(path: &Path, writable: bool) -> Result<DiskRevision> {
    let path = checked_path(path, false)?;
    let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    if !metadata.is_file() || unsupported_link(&metadata) {
        return Err("Native target is not a regular file".into());
    }
    if writable && metadata.permissions().readonly() {
        return Err("Native target is readonly".into());
    }
    if metadata.len() > MAX_NATIVE_SAVE_BYTES as u64 {
        return Err("Native target exceeds16MiB".into());
    }
    let mut file = OpenOptions::new()
        .read(true)
        .write(writable)
        .open(&path)
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    (&mut file)
        .take(MAX_NATIVE_SAVE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 != metadata.len() {
        return Err("Native target changed during read".into());
    }
    // A point-in-time path check only. Matching content does not prove an inode
    // was not replaced with another regular file containing identical bytes.
    let final_metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    if unsupported_link(&final_metadata)
        || !final_metadata.is_file()
        || (writable && final_metadata.permissions().readonly())
    {
        return Err("Native target type/permissions changed".into());
    }
    DiskRevision::from_bytes(&bytes)
}

#[cfg(not(target_arch = "wasm32"))]
fn remove_owned_file(path: &Path) {
    // Only a path exclusively created by this operation is passed here.
    // Never follow a replacement symlink or remove a directory recursively.
    if fs::symlink_metadata(path).is_ok_and(|m| m.is_file() && !unsupported_link(&m)) {
        let _ = fs::remove_file(path);
    }
}
#[cfg(not(target_arch = "wasm32"))]
struct SiblingTemp {
    path: PathBuf,
    file: Option<File>,
    committed: bool,
}
#[cfg(not(target_arch = "wasm32"))]
impl SiblingTemp {
    fn create(target: &Path) -> Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let parent = target.parent().ok_or("Native target parent missing")?;
        let name = target.file_name().ok_or("Native target name missing")?;
        for _ in 0..16 {
            let mut temp = name.to_os_string();
            temp.push(format!(
                ".mathfolio-{}-{}-{}.tmp",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|e| e.to_string())?
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let path = parent.join(temp);
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&path) {
                Ok(file) => {
                    return Ok(Self {
                        path,
                        file: Some(file),
                        committed: false,
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
        Err("Native temporary filename collisions".into())
    }
}
#[cfg(not(target_arch = "wasm32"))]
impl Drop for SiblingTemp {
    fn drop(&mut self) {
        drop(self.file.take());
        if !self.committed {
            remove_owned_file(&self.path);
        }
    }
}

/// Internal ZIP sink prevents output overrun while keeping the existing native
/// writer's exact ordering/options. ZIP header seeks cannot cross the cap.
pub(crate) struct BoundedCursor {
    inner: Cursor<Vec<u8>>,
    limit: u64,
}
impl BoundedCursor {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            inner: Cursor::new(Vec::new()),
            limit: limit as u64,
        }
    }
    pub(crate) fn into_inner(self) -> Vec<u8> {
        self.inner.into_inner()
    }
}
impl Write for BoundedCursor {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self
            .inner
            .position()
            .checked_add(bytes.len() as u64)
            .is_none_or(|end| end > self.limit)
        {
            return Err(std::io::Error::other(
                "Prepared native ZIP exceeds byte cap",
            ));
        }
        self.inner.write(bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}
impl Seek for BoundedCursor {
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        let next = match from {
            SeekFrom::Start(n) => i128::from(n),
            SeekFrom::Current(n) => i128::from(self.inner.position()) + i128::from(n),
            SeekFrom::End(n) => self.inner.get_ref().len() as i128 + i128::from(n),
        };
        if next < 0 || next > i128::from(self.limit) {
            return Err(std::io::Error::other(
                "Prepared native ZIP seek exceeds cap",
            ));
        }
        self.inner.seek(SeekFrom::Start(next as u64))
    }
}

#[cfg(test)]
#[path = "native_save_tests.rs"]
mod tests;
