//! Directory listing and ordering.

use crate::format;
use crate::probe::MediaInfo;
use std::cmp::Ordering;
use std::collections::HashMap;
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

    /// A document that is plain text to read ("Notes.txt", "README.md"),
    /// among the files whose kind is [`Kind::Other`].
    pub fn is_text(&self) -> bool {
        self.kind == Kind::Other && self.extension().is_some_and(is_text_extension)
    }
}

/// Extensions of documents that are read as text.
fn is_text_extension(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "txt"
            | "md"
            | "markdown"
            | "rtf"
            | "nfo"
            | "log"
            | "csv"
            | "json"
            | "xml"
            | "yaml"
            | "yml"
            | "toml"
            | "ini"
            | "cue"
            | "m3u"
            | "m3u8"
            | "pdf"
            | "doc"
            | "docx"
            | "pages"
    )
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
    /// Running time. Needs what a probe found; see [`sort_with`].
    Duration,
    /// The main video stream: its height, then its codec.
    Video,
    /// The main audio stream: its codec, then its channel count.
    Audio,
}

impl SortKey {
    /// What it sorts by, as a column title says it: "Name", "Date Modified".
    pub fn label(self) -> &'static str {
        match self {
            SortKey::Name => "Name",
            SortKey::Size => "Size",
            SortKey::Modified => "Date Modified",
            SortKey::Kind => "Kind",
            SortKey::Duration => "Duration",
            SortKey::Video => "Video",
            SortKey::Audio => "Audio",
        }
    }

    /// Whether ordering by this key needs what a probe found in each file.
    pub fn needs_media(self) -> bool {
        matches!(self, SortKey::Duration | SortKey::Video | SortKey::Audio)
    }
}

/// Sorts in place. Folders always come first; ties fall back to name. The
/// media keys need probe results; without them they sort by name, so use
/// [`sort_with`] for those.
pub fn sort(entries: &mut [Entry], key: SortKey, ascending: bool) {
    sort_with(entries, key, ascending, |_| None);
}

/// Sorts in place, reading media facts through `media` for the keys that
/// need them. Folders always come first. Items with no value for a media key
/// (not media, not read yet, or without that stream) come after those with
/// one, in either direction. Ties fall back to name.
pub fn sort_with<'a>(
    entries: &mut [Entry],
    key: SortKey,
    ascending: bool,
    media: impl Fn(&Path) -> Option<&'a MediaInfo>,
) {
    // Read each file's facts once, not once per comparison.
    let facts: HashMap<PathBuf, MediaFact> = match key.needs_media() {
        true => entries
            .iter()
            .filter(|e| !e.is_dir())
            .filter_map(|e| Some((e.path.clone(), media_fact(key, media(&e.path)?)?)))
            .collect(),
        false => HashMap::new(),
    };
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
            SortKey::Duration | SortKey::Video | SortKey::Audio => {
                match (facts.get(&a.path), facts.get(&b.path)) {
                    (Some(fa), Some(fb)) => fa.cmp(fb),
                    // Nothing to compare goes last, by name, whichever
                    // way the column is sorted.
                    (Some(_), None) => return Ordering::Less,
                    (None, Some(_)) => return Ordering::Greater,
                    (None, None) => return natural_cmp(&a.name, &b.name),
                }
            }
        };
        let ord = ord.then_with(|| natural_cmp(&a.name, &b.name));
        if ascending { ord } else { ord.reverse() }
    });
}

/// What one file is compared by under a media key.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum MediaFact {
    /// Milliseconds, so it orders exactly.
    Duration(u64),
    /// Height, then the codec as it is shown.
    Video(u32, CodecName),
    /// The codec as it is shown, then channels.
    Audio(CodecName, u32),
}

/// A codec's display name, ordered as a person reads it ("AAC" before
/// "AC-3", case ignored).
#[derive(PartialEq, Eq)]
struct CodecName(String);

impl Ord for CodecName {
    fn cmp(&self, other: &Self) -> Ordering {
        natural_cmp(&self.0, &other.0)
    }
}

impl PartialOrd for CodecName {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn media_fact(key: SortKey, info: &MediaInfo) -> Option<MediaFact> {
    match key {
        SortKey::Duration => (info.duration > 0.)
            .then(|| MediaFact::Duration((info.duration * 1000.).round() as u64)),
        SortKey::Video => info.primary_video().map(|video| {
            MediaFact::Video(
                video.height.unwrap_or(0),
                CodecName(format::codec(&video.codec)),
            )
        }),
        SortKey::Audio => info.primary_audio().map(|audio| {
            MediaFact::Audio(
                CodecName(format::codec(&audio.codec)),
                audio.channels.unwrap_or(0),
            )
        }),
        _ => None,
    }
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
    fn text_documents_are_told_apart() {
        let entry = |name: &str, kind| Entry {
            name: name.into(),
            path: PathBuf::from("/media").join(name),
            kind,
            size: 0,
            modified: None,
            hidden: false,
        };
        assert!(entry("Notes.txt", Kind::Other).is_text());
        assert!(entry("README.MD", Kind::Other).is_text());
        assert!(!entry("setup.dmg", Kind::Other).is_text());
        assert!(!entry("film.en.srt", Kind::Subtitle).is_text());
        assert!(!entry("notes.txt", Kind::Folder).is_text());
        assert_eq!(SortKey::Modified.label(), "Date Modified");
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

    fn entry(name: &str, kind: Kind) -> Entry {
        Entry {
            name: name.into(),
            path: PathBuf::from("/media").join(name),
            kind,
            size: 0,
            modified: None,
            hidden: false,
        }
    }

    fn stream(kind: crate::probe::StreamKind, codec: &str) -> crate::probe::Stream {
        crate::probe::Stream {
            index: 0,
            kind,
            codec: codec.into(),
            codec_description: None,
            profile: None,
            language: None,
            title: None,
            flags: Default::default(),
            bit_rate: None,
            duration: None,
            tags: Vec::new(),
            width: None,
            height: None,
            frame_rate: None,
            pixel_format: None,
            bit_depth: None,
            color_space: None,
            color_transfer: None,
            color_primaries: None,
            sample_rate: None,
            channels: None,
            channel_layout: None,
        }
    }

    fn info(duration: f64, streams: Vec<crate::probe::Stream>) -> MediaInfo {
        MediaInfo {
            container: String::new(),
            container_description: String::new(),
            duration,
            bit_rate: 0,
            tags: Vec::new(),
            streams,
            chapters: Vec::new(),
        }
    }

    #[test]
    fn sorts_by_media_with_unknowns_last() {
        use crate::probe::StreamKind::{Audio, Video};
        let video = |height, codec| {
            let mut s = stream(Video, codec);
            s.height = Some(height);
            s
        };
        let audio = |codec, channels| {
            let mut s = stream(Audio, codec);
            s.channels = Some(channels);
            s
        };
        let facts: HashMap<PathBuf, MediaInfo> = [
            (
                "a.mkv",
                info(90., vec![video(1080, "hevc"), audio("aac", 2)]),
            ),
            (
                "b.mp4",
                info(30., vec![video(2160, "h264"), audio("ac3", 6)]),
            ),
            (
                "c.mp4",
                info(600., vec![video(1080, "h264"), audio("aac", 1)]),
            ),
            ("d.mp3", info(200., vec![audio("mp3", 2)])),
        ]
        .into_iter()
        .map(|(name, info)| (PathBuf::from("/media").join(name), info))
        .collect();
        let mut entries = vec![
            entry("unread.mov", Kind::Video),
            entry("d.mp3", Kind::Audio),
            entry("c.mp4", Kind::Video),
            entry("notes.txt", Kind::Other),
            entry("b.mp4", Kind::Video),
            entry("folder", Kind::Folder),
            entry("a.mkv", Kind::Video),
        ];
        let mut names = |key, ascending| {
            sort_with(&mut entries, key, ascending, |p| facts.get(p));
            entries.iter().map(|e| e.name.clone()).collect::<Vec<_>>()
        };
        // Longest first; what has no duration stays at the end.
        assert_eq!(
            names(SortKey::Duration, false),
            [
                "folder",
                "c.mp4",
                "d.mp3",
                "a.mkv",
                "b.mp4",
                "notes.txt",
                "unread.mov"
            ]
        );
        // Tallest first, then codec: HEVC after H.264 at the same height.
        assert_eq!(
            names(SortKey::Video, false),
            [
                "folder",
                "b.mp4",
                "a.mkv",
                "c.mp4",
                "d.mp3",
                "notes.txt",
                "unread.mov"
            ]
        );
        // Codec, then channels: Mono before Stereo.
        assert_eq!(
            names(SortKey::Audio, true),
            [
                "folder",
                "c.mp4",
                "a.mkv",
                "b.mp4",
                "d.mp3",
                "notes.txt",
                "unread.mov"
            ]
        );
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
