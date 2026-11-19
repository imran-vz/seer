//! Directory listing and ordering.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Folder,
    Video,
    Audio,
    Image,
    Subtitle,
    Other,
}

impl Kind {
    pub fn from_extension(ext: &str) -> Kind {
        match ext.to_ascii_lowercase().as_str() {
            "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "ts" | "m2ts"
            | "mts" | "mpg" | "mpeg" | "vob" | "ogv" | "3gp" | "mxf" => Kind::Video,
            "mp3" | "flac" | "wav" | "aac" | "ogg" | "wma" | "m4a" | "opus" | "aiff" | "aif"
            | "alac" | "ac3" | "eac3" | "dts" | "mka" | "wv" | "ape" => Kind::Audio,
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "tiff" | "tif" | "heic" | "heif"
            | "avif" | "svg" | "raw" | "dng" | "cr2" | "cr3" | "nef" | "arw" | "jxl" => Kind::Image,
            "srt" | "ass" | "ssa" | "vtt" | "sub" | "idx" | "sup" => Kind::Subtitle,
            _ => Kind::Other,
        }
    }

    /// True for files FFmpeg can tell us something about.
    pub fn is_probeable(self) -> bool {
        matches!(self, Kind::Video | Kind::Audio)
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Folder => "Folder",
            Kind::Video => "Video",
            Kind::Audio => "Audio",
            Kind::Image => "Image",
            Kind::Subtitle => "Subtitles",
            Kind::Other => "Document",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub kind: Kind,
    /// Zero for folders.
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub hidden: bool,
}

impl Entry {
    pub fn is_dir(&self) -> bool {
        self.kind == Kind::Folder
    }

    pub fn extension(&self) -> Option<&str> {
        if self.is_dir() {
            return None;
        }
        self.path.extension().and_then(|e| e.to_str())
    }
}

/// Lists a directory. Entries that cannot be read are skipped, not fatal.
pub fn list_dir(dir: &Path) -> std::io::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for item in std::fs::read_dir(dir)? {
        let Ok(item) = item else { continue };
        let path = item.path();
        // Follow symlinks so a linked folder behaves like a folder.
        let Ok(meta) = std::fs::metadata(&path).or_else(|_| item.metadata()) else {
            continue;
        };
        let name = item.file_name().to_string_lossy().into_owned();
        let kind = if meta.is_dir() {
            Kind::Folder
        } else {
            path.extension()
                .and_then(|e| e.to_str())
                .map(Kind::from_extension)
                .unwrap_or(Kind::Other)
        };
        entries.push(Entry {
            hidden: name.starts_with('.'),
            size: if meta.is_dir() { 0 } else { meta.len() },
            modified: meta.modified().ok(),
            name,
            path,
            kind,
        });
    }
    Ok(entries)
}

/// Moves files and folders to the Trash.
pub fn trash(paths: &[PathBuf]) -> anyhow::Result<()> {
    #[allow(unused_mut)]
    let mut context = trash::TrashContext::default();
    #[cfg(target_os = "macos")]
    {
        // Ask the file manager directly; the default drives Finder with
        // AppleScript, which is slow and needs an automation permission.
        use trash::macos::{DeleteMethod, TrashContextExtMacos as _};
        context.set_delete_method(DeleteMethod::NsFileManager);
    }
    Ok(context.delete_all(paths)?)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Size,
    Modified,
    Kind,
}

/// Sorts in place. Folders always come first; ties fall back to name.
pub fn sort(entries: &mut [Entry], key: SortKey, ascending: bool) {
    entries.sort_by(|a, b| {
        let folders = b.is_dir().cmp(&a.is_dir());
        if folders != Ordering::Equal {
            return folders;
        }
        let ord = match key {
            SortKey::Name => Ordering::Equal,
            SortKey::Size => a.size.cmp(&b.size),
            SortKey::Modified => a.modified.cmp(&b.modified),
            SortKey::Kind => a.kind.cmp(&b.kind).then_with(|| {
                let (ea, eb) = (a.extension().unwrap_or(""), b.extension().unwrap_or(""));
                natural_cmp(ea, eb)
            }),
        };
        let ord = ord.then_with(|| natural_cmp(&a.name, &b.name));
        if ascending { ord } else { ord.reverse() }
    });
}

/// Case-insensitive comparison that orders digit runs by value, so
/// "Episode 2" sorts before "Episode 10" as it does in Finder.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut ai, mut bi) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(ca), Some(cb)) if ca.is_ascii_digit() && cb.is_ascii_digit() => {
                let na = take_number(&mut ai);
                let nb = take_number(&mut bi);
                // Compare by length first so arbitrarily long runs never overflow.
                let ord = na.len().cmp(&nb.len()).then_with(|| na.cmp(&nb));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(ca), Some(cb)) => {
                let ord = ca.to_lowercase().cmp(cb.to_lowercase());
                if ord != Ordering::Equal {
                    return ord;
                }
                ai.next();
                bi.next();
            }
        }
    }
}

fn take_number(it: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut digits = String::new();
    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
        // Leading zeros carry no value.
        if !(digits.is_empty() && c == '0') {
            digits.push(c);
        }
        it.next();
    }
    digits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_order() {
        let mut names = vec!["ep10", "Ep2", "ep1", "a", "B", "ep02b"];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(names, ["a", "B", "ep1", "Ep2", "ep02b", "ep10"]);
    }

    #[test]
    fn lists_and_sorts_folders_first() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("zeta")).unwrap();
        std::fs::write(dir.path().join("b.mkv"), [0u8; 10]).unwrap();
        std::fs::write(dir.path().join("a.mp3"), [0u8; 20]).unwrap();
        std::fs::write(dir.path().join(".hidden"), []).unwrap();

        let mut entries = list_dir(dir.path()).unwrap();
        sort(&mut entries, SortKey::Size, false);
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["zeta", "a.mp3", "b.mkv", ".hidden"]);
        assert_eq!(entries[1].kind, Kind::Audio);
        assert_eq!(entries[2].kind, Kind::Video);
        assert!(entries[3].hidden);
    }
}

#[cfg(test)]
mod timing {
    use super::*;

    /// Run with `cargo test -p seer-core --release -- --ignored --nocapture listing_speed`.
    #[test]
    #[ignore = "timing, not correctness"]
    fn listing_speed() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..10_000 {
            std::fs::write(dir.path().join(format!("Episode {i} of a show.mkv")), []).unwrap();
        }
        let start = std::time::Instant::now();
        let mut entries = list_dir(dir.path()).unwrap();
        let listed = start.elapsed();
        sort(&mut entries, SortKey::Name, true);
        println!(
            "list {listed:?}, list + sort {:?} for {} entries",
            start.elapsed(),
            entries.len()
        );
        assert_eq!(entries.len(), 10_000);
    }
}
