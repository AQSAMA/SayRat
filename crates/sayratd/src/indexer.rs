// SPDX-License-Identifier: GPL-3.0-or-later

//! Application indexer and small on-disk store.
//!
//! The freedesktop `.desktop` format is INI-like for the fields SayRat needs
//! in Phase 2, so this module deliberately uses a minimal hand-rolled parser
//! instead of `freedesktop_entry_parser`; that crate is not in the approved
//! list yet and would add avoidable dependency surface. Likewise, this offline
//! Phase 2 bootstrap stores postcard-shaped binary records in one file at the
//! requested `index.redb` path; the public API is isolated so swapping the body
//! to `redb` is mechanical once the dependency is available.
//!
//! `notify` is also not available in this environment. The watcher fallback is
//! a periodic 60-second rescan, trading slower change detection for zero extra
//! resident memory and no platform-specific native backend dependency.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sayrat_protocol::codec;
use sayrat_protocol::messages::{Entry, EntryKind};

const MAGIC: &[u8; 8] = b"SRIDX001";
const SCHEMA_VERSION: u64 = 1;

/// Indexer error.
#[derive(Debug)]
pub enum IndexError {
    /// I/O failed.
    Io(std::io::Error),
    /// Codec failed.
    Codec(codec::CodecError),
    /// Store format is invalid.
    InvalidStore(&'static str),
}

impl std::fmt::Display for IndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::Codec(err) => write!(f, "codec error: {err}"),
            Self::InvalidStore(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for IndexError {}

impl From<std::io::Error> for IndexError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<codec::CodecError> for IndexError {
    fn from(value: codec::CodecError) -> Self {
        Self::Codec(value)
    }
}

/// Indexer result.
pub type Result<T> = std::result::Result<T, IndexError>;

/// Index operation.
#[derive(Debug, Clone)]
pub enum IndexOperation {
    /// Rebuild from every configured application directory.
    FullRescan,
    /// Refresh one changed path.
    IncrementalUpdate(PathBuf),
}

/// Durable application index handle.
#[derive(Debug, Clone)]
pub struct AppIndex {
    db_path: PathBuf,
    app_dirs: Vec<PathBuf>,
}

impl AppIndex {
    /// Create an index at `db_path` scanning `app_dirs`.
    pub fn new(db_path: PathBuf, app_dirs: Vec<PathBuf>) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let index = Self { db_path, app_dirs };
        if !index.db_path.exists() {
            index.save(&BTreeMap::new())?;
        }
        Ok(index)
    }

    /// Build paths from XDG-like environment variables.
    pub fn from_environment(socket_override_data_home: Option<PathBuf>) -> Result<Self> {
        let data_home = socket_override_data_home.unwrap_or_else(default_data_home);
        let db_path = data_home.join("sayrat").join("index.redb");
        Self::new(db_path, application_dirs())
    }

    /// Database file path.
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Perform an idempotent index operation.
    pub fn apply(&self, operation: IndexOperation) -> Result<()> {
        match operation {
            IndexOperation::FullRescan => self.full_rescan(),
            IndexOperation::IncrementalUpdate(path) => self.incremental_update(&path),
        }
    }

    /// List entries by id order.
    pub fn list_entries(&self, limit: u16) -> Result<(Vec<Entry>, bool)> {
        let entries = self.load()?;
        let requested = usize::from(limit);
        let mut items = Vec::with_capacity(requested.min(entries.len()));
        for entry in entries.values().take(requested) {
            items.push(entry.clone());
        }
        Ok((items, entries.len() > requested))
    }

    fn full_rescan(&self) -> Result<()> {
        let mut entries = BTreeMap::new();
        for dir in &self.app_dirs {
            scan_dir(dir, &mut entries)?;
        }
        self.save(&entries)
    }

    fn incremental_update(&self, path: &Path) -> Result<()> {
        let mut entries = self.load()?;
        let id = stable_id(path);
        if path.extension().and_then(|ext| ext.to_str()) == Some("desktop") && path.exists() {
            match parse_desktop_file(path)? {
                Some(entry) => {
                    entries.insert(id, entry);
                }
                None => {
                    entries.remove(&id);
                }
            }
        } else {
            entries.remove(&id);
        }
        self.save(&entries)
    }

    fn load(&self) -> Result<BTreeMap<u64, Entry>> {
        let mut file = File::open(&self.db_path)?;
        let mut magic = [0_u8; 8];
        file.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(IndexError::InvalidStore("bad index magic"));
        }
        let schema = read_u64(&mut file)?;
        if schema != SCHEMA_VERSION {
            return Err(IndexError::InvalidStore("unsupported schema"));
        }
        let _last_scan = read_u64(&mut file)?;
        let count = read_u64(&mut file)?;
        let mut entries = BTreeMap::new();
        for _ in 0..count {
            let entry: Entry = codec::read_message(&mut file)?;
            entries.insert(entry.id, entry);
        }
        Ok(entries)
    }

    fn save(&self, entries: &BTreeMap<u64, Entry>) -> Result<()> {
        if let Some(parent) = self.db_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = self.db_path.with_extension("redb.tmp");
        let mut file = File::create(&tmp)?;
        file.write_all(MAGIC)?;
        write_u64(&mut file, SCHEMA_VERSION)?;
        write_u64(&mut file, unix_now())?;
        write_u64(&mut file, entries.len() as u64)?;
        for entry in entries.values() {
            codec::write_message(&mut file, entry)?;
        }
        file.flush()?;
        fs::rename(tmp, &self.db_path)?;
        Ok(())
    }
}

/// Start a fallback watcher thread that periodically performs a full rescan.
pub fn spawn_periodic_watcher(index: AppIndex) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(60));
            if let Err(err) = index.apply(IndexOperation::FullRescan) {
                log::warn!("periodic application rescan failed: {err}");
            }
        }
    })
}

/// Standard application directories for this platform.
pub fn application_dirs() -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let mut dirs = Vec::new();
        if let Some(program_data) = std::env::var_os("ProgramData") {
            dirs.push(
                PathBuf::from(program_data)
                    .join("Microsoft")
                    .join("Windows")
                    .join("Start Menu")
                    .join("Programs"),
            );
        }
        if let Some(app_data) = std::env::var_os("APPDATA") {
            dirs.push(
                PathBuf::from(app_data)
                    .join("Microsoft")
                    .join("Windows")
                    .join("Start Menu")
                    .join("Programs"),
            );
        }
        dirs
    }
    #[cfg(target_os = "macos")]
    {
        let mut dirs = vec![PathBuf::from("/Applications")];
        if let Some(home) = std::env::var_os("HOME") {
            dirs.push(PathBuf::from(home).join("Applications"));
        }
        dirs
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let data_home =
            std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(default_data_home);
        let mut dirs = vec![data_home.join("applications")];
        let data_dirs = std::env::var_os("XDG_DATA_DIRS")
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_else(|| String::from("/usr/local/share:/usr/share"));
        for dir in data_dirs.split(':').filter(|dir| !dir.is_empty()) {
            dirs.push(PathBuf::from(dir).join("applications"));
        }
        dirs
    }
}

fn default_data_home() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn scan_dir(dir: &Path, entries: &mut BTreeMap<u64, Entry>) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for item in fs::read_dir(dir)? {
        let item = item?;
        let path = item.path();
        if path.is_dir() {
            scan_dir(&path, entries)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("desktop")
            && let Some(entry) = parse_desktop_file(&path)?
        {
            entries.insert(entry.id, entry);
        }
    }
    Ok(())
}

fn parse_desktop_file(path: &Path) -> Result<Option<Entry>> {
    let content = fs::read_to_string(path)?;
    let mut in_desktop_entry = false;
    let mut name = None;
    let mut comment = None;
    let mut exec = None;
    let mut icon = None;
    let mut hidden = false;
    let mut no_display = false;
    let mut entry_type = None;

    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_desktop_entry {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "Type" => entry_type = Some(value.trim().to_owned()),
            "Name" => name = Some(value.trim().to_owned()),
            "Comment" | "GenericName" if comment.is_none() => {
                comment = Some(value.trim().to_owned());
            }
            "Exec" => exec = Some(value.trim().to_owned()),
            "Icon" => icon = Some(value.trim().to_owned()),
            "Hidden" => hidden = value.trim().eq_ignore_ascii_case("true"),
            "NoDisplay" => no_display = value.trim().eq_ignore_ascii_case("true"),
            _ => {}
        }
    }

    if hidden || no_display || entry_type.as_deref() != Some("Application") {
        return Ok(None);
    }
    let Some(name) = name else {
        return Ok(None);
    };
    Ok(Some(Entry {
        id: stable_id(path),
        kind: EntryKind::Application,
        name,
        subtitle: comment,
        exec,
        icon,
    }))
}

fn stable_id(path: &Path) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in path.to_string_lossy().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|duration| duration.as_secs()).unwrap_or(0)
}

fn read_u64(reader: &mut impl Read) -> Result<u64> {
    let mut bytes = [0_u8; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

fn write_u64(writer: &mut impl Write, value: u64) -> Result<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_desktop_files_are_indexed() {
        let base = unique_temp_dir("indexer");
        let apps = base.join("applications");
        fs::create_dir_all(&apps).unwrap_or_else(|err| panic!("create apps: {err}"));
        fs::write(
            apps.join("one.desktop"),
            "[Desktop Entry]\nType=Application\nName=One\nComment=First\nExec=one\nIcon=one\n",
        )
        .unwrap_or_else(|err| panic!("write one: {err}"));
        fs::write(
            apps.join("two.desktop"),
            "[Desktop Entry]\nType=Application\nName=Two\nGenericName=Second\nExec=two\n",
        )
        .unwrap_or_else(|err| panic!("write two: {err}"));

        let index = AppIndex::new(base.join("index.redb"), vec![apps])
            .unwrap_or_else(|err| panic!("index new: {err}"));
        index.apply(IndexOperation::FullRescan).unwrap_or_else(|err| panic!("rescan: {err}"));
        let (entries, more) = index.list_entries(10).unwrap_or_else(|err| panic!("list: {err}"));
        assert!(!more);
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|entry| entry.name == "One"));
        assert!(entries.iter().any(|entry| entry.name == "Two"));
        fs::remove_dir_all(base).unwrap_or_else(|err| panic!("cleanup: {err}"));
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("sayrat-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        path
    }
}
