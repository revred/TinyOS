//! The labelled RAM volume (`FEAT-P2-02`, `STORY-P2-02-01`) — the `LE-48` answer.
//!
//! Fixed capacities, no heap, fail-closed: every exhaustion and every malformed path is a
//! typed [`VolumeError`], never a panic. `G-SEC-5` labels travel with every file;
//! [`RamVolume::copy`] and [`RamVolume::rename`] propagate them bit-for-bit and record
//! the transform in the derivation bits — a transform can add history, never shed it.

use crate::capacities::{MAX_DATA, MAX_DIRS, MAX_FILES, MAX_NAME, MAX_PATH};
use crate::labels::{Labels, DERIVED_COPIED, DERIVED_RENAMED};

/// A bounded name component, matched case-insensitively (DOS ergonomics).
#[derive(Debug, Clone, Copy)]
pub struct Name {
    bytes: [u8; MAX_NAME],
    len: u8,
}

impl Name {
    /// Parse a component; refuses empty, over-length, separators and `.`/`..`.
    pub fn new(text: &str) -> Result<Self, VolumeError> {
        let bytes = text.as_bytes();
        if bytes.is_empty() || bytes.len() > MAX_NAME {
            return Err(VolumeError::BadName);
        }
        if text == "." || text == ".." || text.contains(['\\', '/', ':']) {
            return Err(VolumeError::BadName);
        }
        let mut store = [0u8; MAX_NAME];
        store[..bytes.len()].copy_from_slice(bytes);
        Ok(Name { bytes: store, len: bytes.len() as u8 })
    }
    /// The component as text.
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len as usize]).unwrap_or("?")
    }
    fn eq_ignore_case(&self, other: &str) -> bool {
        self.as_str().eq_ignore_ascii_case(other)
    }
}

/// Everything that can go wrong, each mapping to a register message shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeError {
    /// `File not found`.
    NotFound,
    /// `Invalid directory`.
    BadDirectory,
    /// `Directory already exists` / duplicate name.
    Exists,
    /// A fixed capacity is exhausted — `Insufficient disk space`.
    Full,
    /// `Invalid path, not directory, or directory not empty`.
    NotEmpty,
    /// Malformed or escaping path — `Invalid path`.
    BadPath,
    /// Malformed name component.
    BadName,
    /// Content too large for a file slot — `Insufficient disk space`.
    TooLarge,
    /// Refused: quarantined content (`G-SEC-5`).
    Quarantined,
    /// Refused: read-only entitlement — `Access denied `.
    ReadOnly,
}

struct DirSlot {
    name: Name,
    parent: u8,
}

struct FileSlot {
    name: Name,
    dir: u8,
    len: u16,
    data: [u8; MAX_DATA],
    /// The `G-SEC-5` label set, present from creation.
    labels: Labels,
}

/// One directory-listing entry.
pub struct Entry<'a> {
    /// Component name.
    pub name: &'a str,
    /// `None` for directories, `Some(bytes)` for files.
    pub size: Option<u16>,
    /// File labels (directories carry none in this slice).
    pub labels: Option<Labels>,
}

/// The volume: root is directory index 0.
pub struct RamVolume {
    label: Option<Name>,
    /// Volume serial, DOS-style two halves.
    pub serial: (u16, u16),
    dirs: [Option<DirSlot>; MAX_DIRS],
    files: [Option<FileSlot>; MAX_FILES],
}

const NO_DIR: Option<DirSlot> = None;
const NO_FILE: Option<FileSlot> = None;

impl RamVolume {
    /// An empty volume with a fixed serial (deterministic transcripts).
    pub fn new(label: Option<&str>, serial: (u16, u16)) -> Self {
        RamVolume {
            label: label.and_then(|l| Name::new(l).ok()),
            serial,
            dirs: [NO_DIR; MAX_DIRS],
            files: [NO_FILE; MAX_FILES],
        }
    }

    /// The volume label, if set.
    pub fn label(&self) -> Option<&str> {
        self.label.as_ref().map(|n| n.as_str())
    }

    /// Resolve `path` (absolute `\...` or relative to `cwd_dir`) to a directory index.
    /// `..` walks up but refuses to escape the root; drive prefixes other than `A:` refuse.
    pub fn resolve_dir(&self, cwd_dir: u8, path: &str) -> Result<u8, VolumeError> {
        let mut rest = path;
        if let Some(stripped) = rest.strip_prefix("A:").or_else(|| rest.strip_prefix("a:")) {
            rest = stripped;
        } else if rest.len() >= 2 && rest.as_bytes()[1] == b':' {
            return Err(VolumeError::BadPath);
        }
        let mut current = if rest.starts_with(['\\', '/']) { 0 } else { cwd_dir };
        for component in rest.split(['\\', '/']).filter(|c| !c.is_empty()) {
            match component {
                "." => {}
                ".." => {
                    if current == 0 {
                        return Err(VolumeError::BadPath);
                    }
                    current = self.dirs[current as usize - 1]
                        .as_ref()
                        .ok_or(VolumeError::BadDirectory)?
                        .parent;
                }
                name => {
                    current = self.find_subdir(current, name).ok_or(VolumeError::BadDirectory)?;
                }
            }
        }
        Ok(current)
    }

    /// Split `path` into (directory index, final component).
    fn resolve_parent<'a>(&self, cwd_dir: u8, path: &'a str) -> Result<(u8, &'a str), VolumeError> {
        let trimmed = path.trim_end_matches(['\\', '/']);
        match trimmed.rfind(['\\', '/']) {
            Some(split) => {
                let (dir_part, name) = trimmed.split_at(split);
                let dir_part = if dir_part.is_empty() { "\\" } else { dir_part };
                Ok((self.resolve_dir(cwd_dir, dir_part)?, &name[1..]))
            }
            None => {
                let mut rest = trimmed;
                if let Some(s) = rest.strip_prefix("A:").or_else(|| rest.strip_prefix("a:")) {
                    rest = s;
                }
                Ok((cwd_dir, rest))
            }
        }
    }

    fn find_subdir(&self, parent: u8, name: &str) -> Option<u8> {
        (0..MAX_DIRS).find_map(|slot| {
            let dir = self.dirs[slot].as_ref()?;
            (dir.parent == parent && dir.name.eq_ignore_case(name)).then_some(slot as u8 + 1)
        })
    }

    fn find_file(&self, dir: u8, name: &str) -> Option<usize> {
        (0..MAX_FILES)
            .find(|&slot| matches!(&self.files[slot], Some(f) if f.dir == dir && f.name.eq_ignore_case(name)))
    }

    /// Render `dir`'s absolute path into `out`; returns the length.
    pub fn dir_path(&self, dir: u8, out: &mut [u8; MAX_PATH]) -> usize {
        if dir == 0 {
            out[0] = b'\\';
            return 1;
        }
        let mut chain = [0u8; MAX_DIRS];
        let mut depth = 0;
        let mut current = dir;
        while current != 0 && depth < MAX_DIRS {
            chain[depth] = current;
            depth += 1;
            current = self.dirs[current as usize - 1].as_ref().map(|d| d.parent).unwrap_or(0);
        }
        let mut len = 0;
        for index in (0..depth).rev() {
            let name = self.dirs[chain[index] as usize - 1].as_ref().unwrap().name;
            let text = name.as_str().as_bytes();
            if len + 1 + text.len() > MAX_PATH {
                break;
            }
            out[len] = b'\\';
            len += 1;
            out[len..len + text.len()].copy_from_slice(text);
            len += text.len();
        }
        len
    }

    /// `MD`: create a subdirectory.
    pub fn mkdir(&mut self, cwd: u8, path: &str) -> Result<(), VolumeError> {
        let (parent, name) = self.resolve_parent(cwd, path)?;
        if self.find_subdir(parent, name).is_some() || self.find_file(parent, name).is_some() {
            return Err(VolumeError::Exists);
        }
        let name = Name::new(name)?;
        let slot = (0..MAX_DIRS).find(|&s| self.dirs[s].is_none()).ok_or(VolumeError::Full)?;
        self.dirs[slot] = Some(DirSlot { name, parent });
        Ok(())
    }

    /// `RD`: remove an empty subdirectory.
    pub fn rmdir(&mut self, cwd: u8, path: &str) -> Result<(), VolumeError> {
        let index = self.resolve_dir(cwd, path).map_err(|_| VolumeError::NotEmpty)?;
        if index == 0 || index == cwd {
            return Err(VolumeError::NotEmpty);
        }
        let has_children = (0..MAX_DIRS)
            .any(|s| matches!(&self.dirs[s], Some(d) if d.parent == index))
            || (0..MAX_FILES).any(|s| matches!(&self.files[s], Some(f) if f.dir == index));
        if has_children {
            return Err(VolumeError::NotEmpty);
        }
        self.dirs[index as usize - 1] = None;
        Ok(())
    }

    /// Create a file with `labels` carried from birth.
    pub fn create(
        &mut self,
        cwd: u8,
        path: &str,
        content: &[u8],
        labels: Labels,
    ) -> Result<(), VolumeError> {
        if content.len() > MAX_DATA {
            return Err(VolumeError::TooLarge);
        }
        let (dir, name) = self.resolve_parent(cwd, path)?;
        if self.find_file(dir, name).is_some() || self.find_subdir(dir, name).is_some() {
            return Err(VolumeError::Exists);
        }
        let name = Name::new(name)?;
        let slot = (0..MAX_FILES).find(|&s| self.files[s].is_none()).ok_or(VolumeError::Full)?;
        let mut data = [0u8; MAX_DATA];
        data[..content.len()].copy_from_slice(content);
        self.files[slot] = Some(FileSlot { name, dir, len: content.len() as u16, data, labels });
        Ok(())
    }

    /// `TYPE`: read a file's bytes. Quarantined content refuses.
    pub fn read(&self, cwd: u8, path: &str) -> Result<&[u8], VolumeError> {
        let (dir, name) = self.resolve_parent(cwd, path)?;
        let slot = self.find_file(dir, name).ok_or(VolumeError::NotFound)?;
        let file = self.files[slot].as_ref().unwrap();
        if file.labels.quarantine {
            return Err(VolumeError::Quarantined);
        }
        Ok(&file.data[..file.len as usize])
    }

    /// A file's labels (readable even when quarantined — inspection is not consumption).
    pub fn stat(&self, cwd: u8, path: &str) -> Result<Labels, VolumeError> {
        let (dir, name) = self.resolve_parent(cwd, path)?;
        let slot = self.find_file(dir, name).ok_or(VolumeError::NotFound)?;
        Ok(self.files[slot].as_ref().unwrap().labels)
    }

    /// `DEL`: remove a file. Read-only entitlement refuses.
    pub fn delete(&mut self, cwd: u8, path: &str) -> Result<(), VolumeError> {
        let (dir, name) = self.resolve_parent(cwd, path)?;
        let slot = self.find_file(dir, name).ok_or(VolumeError::NotFound)?;
        if self.files[slot].as_ref().unwrap().labels.read_only {
            return Err(VolumeError::ReadOnly);
        }
        self.files[slot] = None;
        Ok(())
    }

    /// `COPY`: duplicate content **and labels**; derivation gains `DERIVED_COPIED`.
    pub fn copy(&mut self, cwd: u8, src: &str, dst: &str) -> Result<(), VolumeError> {
        let (sdir, sname) = self.resolve_parent(cwd, src)?;
        let slot = self.find_file(sdir, sname).ok_or(VolumeError::NotFound)?;
        let source = self.files[slot].as_ref().unwrap();
        let mut labels = source.labels;
        labels.derivation |= DERIVED_COPIED;
        let (content, len) = (source.data, source.len);
        self.create(cwd, dst, &content[..len as usize], labels)
    }

    /// `REN`/`MOVE`: same slot, new home; labels intact plus `DERIVED_RENAMED`.
    pub fn rename(&mut self, cwd: u8, src: &str, dst: &str) -> Result<(), VolumeError> {
        let (sdir, sname) = self.resolve_parent(cwd, src)?;
        let slot = self.find_file(sdir, sname).ok_or(VolumeError::NotFound)?;
        let (ddir, dname) = self.resolve_parent(cwd, dst)?;
        if self.find_file(ddir, dname).is_some() || self.find_subdir(ddir, dname).is_some() {
            return Err(VolumeError::Exists);
        }
        let new_name = Name::new(dname)?;
        let file = self.files[slot].as_mut().unwrap();
        file.name = new_name;
        file.dir = ddir;
        file.labels.derivation |= DERIVED_RENAMED;
        Ok(())
    }

    /// Directory listing in slot (insertion) order: subdirectories, then files.
    pub fn list(&self, dir: u8) -> impl Iterator<Item = Entry<'_>> {
        let dirs = self.dirs.iter().filter_map(move |slot| {
            let entry = slot.as_ref()?;
            (entry.parent == dir).then_some(Entry {
                name: entry.name.as_str(),
                size: None,
                labels: None,
            })
        });
        let files = self.files.iter().filter_map(move |slot| {
            let file = slot.as_ref()?;
            (file.dir == dir).then_some(Entry {
                name: file.name.as_str(),
                size: Some(file.len),
                labels: Some(file.labels),
            })
        });
        dirs.chain(files)
    }

    /// Subdirectory indices of `dir`, for `TREE`.
    pub fn subdirs(&self, dir: u8) -> impl Iterator<Item = u8> + '_ {
        (0..MAX_DIRS).filter_map(move |slot| {
            let entry = self.dirs[slot].as_ref()?;
            (entry.parent == dir).then_some(slot as u8 + 1)
        })
    }

    /// Free bytes: unused file slots × slot size (deterministic, honest about the model).
    pub fn free_bytes(&self) -> u32 {
        let used = self.files.iter().filter(|f| f.is_some()).count();
        ((MAX_FILES - used) * MAX_DATA) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::labels::{Labels, DERIVED_COPIED, DERIVED_RENAMED};

    fn volume() -> RamVolume {
        RamVolume::new(Some("TINYOS"), (0x1234, 0xABCD))
    }

    /// V1 — create/read/list round-trip with labels present from birth.
    #[test]
    fn v1_create_read_list() {
        let mut vol = volume();
        vol.create(0, "HELLO.TXT", b"hi", Labels::seeded()).unwrap();
        assert_eq!(vol.read(0, "hello.txt").unwrap(), b"hi");
        let entries: Vec<_> = vol.list(0).map(|e| e.name.to_string()).collect();
        assert_eq!(entries, ["HELLO.TXT"]);
        assert_eq!(vol.stat(0, "HELLO.TXT").unwrap(), Labels::seeded());
    }

    /// V2 — labels survive copy→rename→copy; derivation accumulates; quarantine sticks
    /// (STORY-P2-02-01 acceptance 2).
    #[test]
    fn v2_labels_survive_transform_chains() {
        let mut vol = volume();
        let mut quarantined = Labels::seeded();
        quarantined.quarantine = true;
        vol.create(0, "EVIL.TCB", b"del *", quarantined).unwrap();

        vol.copy(0, "EVIL.TCB", "COPY1.TCB").unwrap();
        vol.rename(0, "COPY1.TCB", "SAFE.TXT").unwrap();
        vol.copy(0, "SAFE.TXT", "FINAL.TXT").unwrap();

        let labels = vol.stat(0, "FINAL.TXT").unwrap();
        assert!(labels.quarantine, "quarantine must survive copy-rename-copy");
        assert_eq!(labels.derivation, DERIVED_COPIED | DERIVED_RENAMED);
        assert_eq!(vol.read(0, "FINAL.TXT").unwrap_err(), VolumeError::Quarantined);
    }

    /// V3 — traversal refuses to escape the root (STORY-P2-02-01 acceptance 3).
    #[test]
    fn v3_traversal_refused() {
        let mut vol = volume();
        vol.mkdir(0, "DOCS").unwrap();
        assert_eq!(vol.resolve_dir(0, "..\\..\\.."), Err(VolumeError::BadPath));
        assert_eq!(vol.resolve_dir(0, "\\..\\X"), Err(VolumeError::BadPath));
        assert_eq!(vol.create(0, "B:\\X.TXT", b"x", Labels::seeded()), Err(VolumeError::BadPath));
        let docs = vol.resolve_dir(0, "DOCS").unwrap();
        assert_eq!(vol.resolve_dir(docs, ".."), Ok(0));
    }

    /// V4 — every capacity exhausts as a typed refusal and the volume stays usable
    /// (STORY-P2-02-01 acceptance 1).
    #[test]
    fn v4_exhaustion_fails_closed() {
        let mut vol = volume();
        for index in 0..crate::capacities::MAX_FILES {
            vol.create(0, &format!("F{index}.TXT"), b"x", Labels::seeded()).unwrap();
        }
        assert_eq!(vol.create(0, "ONE-MORE.TXT", b"x", Labels::seeded()), Err(VolumeError::Full));
        let big = [0u8; crate::capacities::MAX_DATA + 1];
        vol.delete(0, "F0.TXT").unwrap();
        assert_eq!(vol.create(0, "BIG.BIN", &big, Labels::seeded()), Err(VolumeError::TooLarge));
        vol.create(0, "OK.TXT", b"still works", Labels::seeded()).unwrap();
    }

    /// V5 — read-only entitlement refuses delete; rmdir refuses non-empty.
    #[test]
    fn v5_read_only_and_non_empty_refuse() {
        let mut vol = volume();
        let mut read_only = Labels::seeded();
        read_only.read_only = true;
        vol.create(0, "LOCKED.SYS", b"x", read_only).unwrap();
        assert_eq!(vol.delete(0, "LOCKED.SYS"), Err(VolumeError::ReadOnly));

        vol.mkdir(0, "DOCS").unwrap();
        vol.create(0, "DOCS\\A.TXT", b"a", Labels::seeded()).unwrap();
        assert_eq!(vol.rmdir(0, "DOCS"), Err(VolumeError::NotEmpty));
    }
}
