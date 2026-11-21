//! Finds stream names that say nothing true about their stream.
//!
//! Release sites stamp their address into the title of every stream. A
//! title is fine when it is empty or only describes the stream it sits on
//! ("1080p H.264", "5.1 AC-3 384 kbps", "English"). Anything else is flagged,
//! with a standard name offered in its place.

use crate::format;
use crate::fs::Kind;
use crate::probe::{MediaInfo, Stream, StreamKind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Which name in the file an issue is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    /// The title of the file as a whole.
    Container,
    Stream(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// The name contains a web address.
    WebAddress,
    /// The name has words that do not describe the stream, or contradict it.
    Unrelated,
}

impl Reason {
    pub fn label(self) -> &'static str {
        match self {
            Reason::WebAddress => "Contains a web address",
            Reason::Unrelated => "Doesn’t describe the stream",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub target: Target,
    /// What the name belongs to, e.g. "Audio · AC-3".
    pub label: String,
    pub current: String,
    pub reason: Reason,
    /// A name built from the stream's own facts. `None` when the only
    /// sensible fix is to remove the name.
    pub standard: Option<String>,
}

/// Every name in the file that should be looked at.
pub fn review(info: &MediaInfo) -> Vec<Issue> {
    let mut issues = Vec::new();
    let container_title = info
        .tags
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("title"))
        .map(|(_, v)| v);
    if let Some(title) = container_title.filter(|t| has_web_address(t)) {
        issues.push(Issue {
            target: Target::Container,
            label: "File title".into(),
            current: title.clone(),
            reason: Reason::WebAddress,
            standard: None,
        });
    }
    for stream in &info.streams {
        let Some(title) = stream.title.as_deref().filter(|t| !t.trim().is_empty()) else {
            continue;
        };
        let reason = if has_web_address(title) {
            Reason::WebAddress
        } else if describes(title, stream) {
            continue;
        } else {
            Reason::Unrelated
        };
        issues.push(Issue {
            target: Target::Stream(stream.index),
            label: format!("{} · {}", stream.kind.label(), format::codec(&stream.codec)),
            current: title.to_owned(),
            reason,
            standard: standard_name(stream),
        });
    }
    issues
}

/// The name a stream gets when it is named after its own facts.
pub fn standard_name(stream: &Stream) -> Option<String> {
    let codec = format::codec(&stream.codec);
    match stream.kind {
        StreamKind::Video if stream.flags.attached_picture => None,
        StreamKind::Video => {
            let size = match (stream.width, stream.height) {
                (Some(w), Some(h)) => Some(
                    format::resolution_class(w, h)
                        .map(str::to_owned)
                        .unwrap_or(format!("{w}×{h}")),
                ),
                _ => None,
            };
            let mut parts: Vec<String> = size.into_iter().chain([codec]).collect();
            parts.extend(stream.dynamic_range().map(str::to_owned));
            Some(parts.join(" "))
        }
        // Layout first, then the codec, as the file list's Audio column
        // reads ("Mono AAC"), the way video leads with its size.
        StreamKind::Audio => {
            let mut parts: Vec<String> =
                stream.channels.map(format::channels).into_iter().collect();
            parts.push(codec);
            parts.extend(stream.bit_rate.map(format::bitrate));
            Some(parts.join(" "))
        }
        StreamKind::Subtitle => {
            let mut name = format::language(stream.language.as_deref()?);
            if stream.flags.hearing_impaired {
                name.push_str(" SDH");
            }
            if stream.flags.forced {
                name.push_str(" Forced");
            }
            Some(name)
        }
        StreamKind::Attachment | StreamKind::Data => None,
    }
}

const TLDS: &[&str] = &[
    "com", "net", "org", "io", "to", "cc", "tv", "me", "in", "co", "xyz", "info", "site", "club",
    "ws", "mx", "app", "dev", "link", "live", "fun", "top", "vip", "pro", "one",
];

pub fn has_web_address(text: &str) -> bool {
    text.split(|c: char| c.is_whitespace() || "[](){}<>|,".contains(c))
        .any(|word| {
            let word = word.to_lowercase();
            let word = word.trim_matches(|c: char| !c.is_alphanumeric());
            if word.starts_with("www.")
                || word.starts_with("http://")
                || word.starts_with("https://")
            {
                return true;
            }
            match word.rsplit_once('.') {
                Some((host, tld)) => {
                    TLDS.contains(&tld) && host.len() >= 2 && host.chars().any(char::is_alphabetic)
                }
                None => false,
            }
        })
}

/// Words that describe streams in general and never contradict one.
const NEUTRAL: &[&str] = &[
    "audio",
    "video",
    "track",
    "stream",
    "subtitle",
    "subtitles",
    "subs",
    "sub",
    "surround",
    "hdr",
    "hdr10",
    "sdr",
    "10bit",
    "8bit",
    "bit",
    "kbps",
    "kb/s",
    "mbps",
    "khz",
    "hz",
    "fps",
    "ch",
    "channels",
    "channel",
    "original",
    "atmos",
    "full",
    "sdh",
    "forced",
    "cc",
    "default",
    "main",
    "hd",
    "ma",
    "lossless",
];

/// Names for each codec, with punctuation removed. A word naming a codec
/// other than the stream's own is a contradiction.
const CODEC_WORDS: &[(&str, &[&str])] = &[
    ("h264", &["h264", "x264", "avc"]),
    ("hevc", &["hevc", "h265", "x265"]),
    ("av1", &["av1"]),
    ("vp9", &["vp9"]),
    ("mpeg4", &["mpeg4", "xvid", "divx"]),
    ("mpeg2video", &["mpeg2"]),
    ("ac3", &["ac3", "dd", "dolbydigital"]),
    ("eac3", &["eac3", "ddp", "dd+", "ddplus"]),
    ("dts", &["dts", "dtshd", "dtshdma", "dtsx"]),
    ("truehd", &["truehd"]),
    ("aac", &["aac", "heaac"]),
    ("flac", &["flac"]),
    ("opus", &["opus"]),
    ("mp3", &["mp3"]),
    ("vorbis", &["vorbis"]),
    ("subrip", &["srt", "subrip"]),
    ("ass", &["ass", "ssa"]),
    ("hdmv_pgs_subtitle", &["pgs"]),
];

/// True when every word of the title is about this stream and agrees with it.
fn describes(title: &str, stream: &Stream) -> bool {
    // Commentary and similar tracks are named by people for people.
    if stream.flags.commentary || title.to_lowercase().contains("commentary") {
        return true;
    }
    title
        .split(|c: char| c.is_whitespace() || "[](){}|,·:;/".contains(c))
        .map(|w| w.trim_matches(|c: char| "-–—&".contains(c)).to_lowercase())
        .filter(|w| !w.is_empty())
        .all(|word| word_fits(&word, stream))
}

fn word_fits(word: &str, stream: &Stream) -> bool {
    if NEUTRAL.contains(&word) {
        return true;
    }
    // Language, by name or code.
    if let Some(code) = &stream.language
        && (word == code.to_lowercase() || word == format::language(code).to_lowercase())
    {
        return true;
    }
    // Picture size.
    if let (Some(w), Some(h)) = (stream.width, stream.height) {
        let class = format::resolution_class(w, h).map(str::to_lowercase);
        if class.as_deref() == Some(word)
            || word == format!("{w}x{h}")
            || word == format!("{w}×{h}")
        {
            return true;
        }
        if class.as_deref() == Some("4k") && (word == "2160p" || word == "uhd") {
            return true;
        }
    }
    // Channels.
    let channels = match word {
        "mono" | "1.0" => Some(1),
        "stereo" | "2.0" => Some(2),
        "5.1" => Some(6),
        "7.1" => Some(8),
        _ => None,
    };
    if let Some(count) = channels {
        return stream.channels == Some(count);
    }
    // Bit rate, within a tenth, or a bare number that matches a known fact.
    let digits = word.trim_end_matches("kbps").trim_end_matches("k");
    if let Ok(number) = digits.parse::<f64>() {
        let near = |fact: f64| fact > 0. && (number - fact).abs() <= fact * 0.1;
        let facts = [
            stream.bit_rate.map(|b| b as f64 / 1000.),
            stream.bit_rate.map(|b| b as f64 / 1e6),
            stream.sample_rate.map(|r| r as f64 / 1000.),
            stream.sample_rate.map(f64::from),
            stream.frame_rate,
            stream.height.map(f64::from),
            stream.width.map(f64::from),
            stream.bit_depth.map(f64::from),
        ];
        return facts.into_iter().flatten().any(near)
            || (stream.bit_rate.is_none() && word.ends_with("kbps"));
    }
    // Codec, alone or joined to a channel count ("dd5.1", "aac2.0").
    let squashed: String = word.chars().filter(|c| !"-._".contains(*c)).collect();
    let own = CODEC_WORDS
        .iter()
        .find(|(codec, _)| *codec == stream.codec)
        .map(|(_, words)| *words);
    let own_pcm = stream.codec.starts_with("pcm_") && squashed == "pcm";
    if own_pcm || own.is_some_and(|words| words.contains(&squashed.as_str())) {
        return true;
    }
    if let Some(words) = own {
        for name in words {
            if let Some(rest) = squashed.strip_prefix(name) {
                let count = match rest {
                    "10" => Some(1),
                    "20" => Some(2),
                    "51" => Some(6),
                    "71" => Some(8),
                    _ => None,
                };
                if count.is_some() && count == stream.channels {
                    return true;
                }
            }
        }
    }
    false
}

/// What [`video_files`] found under a folder: every video, and every place
/// it could not look into, so a result never passes off an incomplete walk
/// as a complete one.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Videos {
    /// Every video file, in a stable order.
    pub files: Vec<PathBuf>,
    /// Folders that could not be listed, or not to the end, and items whose
    /// kind could not be read. Videos in them were not found.
    pub unread: Vec<Unread>,
}

/// A place [`video_files`] could not look into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unread {
    pub path: PathBuf,
    /// A folder that could not be listed (or not completely); otherwise an
    /// item whose kind could not be read.
    pub folder: bool,
}

/// Every video file under `root`, at any depth, in a stable order, and
/// everything that could not be read on the way. Hidden items and symbolic
/// links to folders are skipped on purpose and are not counted as unread.
pub fn video_files(root: &Path, cancel: &AtomicBool) -> Videos {
    let mut found = Videos::default();
    let mut pending = vec![root.to_owned()];
    while let Some(dir) = pending.pop() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            found.unread.push(Unread {
                path: dir,
                folder: true,
            });
            continue;
        };
        let mut incomplete = false;
        for entry in entries {
            let Ok(entry) = entry else {
                incomplete = true;
                continue;
            };
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                found.unread.push(Unread {
                    path,
                    folder: false,
                });
                continue;
            };
            if kind.is_dir() {
                pending.push(path);
            } else if kind.is_file()
                && path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(Kind::from_extension)
                    == Some(Kind::Video)
            {
                found.files.push(path);
            }
        }
        if incomplete {
            found.unread.push(Unread {
                path: dir,
                folder: true,
            });
        }
    }
    let order = |a: &PathBuf, b: &PathBuf| {
        crate::fs::natural_cmp(&a.to_string_lossy(), &b.to_string_lossy())
    };
    found.files.sort_by(order);
    found.unread.sort_by(|a, b| order(&a.path, &b.path));
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::StreamFlags;

    fn stream(index: usize, kind: StreamKind, codec: &str, title: &str) -> Stream {
        Stream {
            index,
            kind,
            codec: codec.into(),
            codec_description: None,
            profile: None,
            language: None,
            title: (!title.is_empty()).then(|| title.to_owned()),
            flags: StreamFlags::default(),
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

    fn video(title: &str) -> Stream {
        Stream {
            width: Some(1920),
            height: Some(800),
            frame_rate: Some(25.),
            ..stream(0, StreamKind::Video, "h264", title)
        }
    }

    fn audio(title: &str) -> Stream {
        Stream {
            language: Some("tam".into()),
            channels: Some(6),
            bit_rate: Some(384_000),
            sample_rate: Some(48_000),
            ..stream(1, StreamKind::Audio, "ac3", title)
        }
    }

    fn file(streams: Vec<Stream>, title: Option<&str>) -> MediaInfo {
        MediaInfo {
            container: "matroska".into(),
            container_description: "Matroska".into(),
            duration: 60.,
            bit_rate: 0,
            tags: title
                .map(|t| ("title".to_owned(), t.to_owned()))
                .into_iter()
                .collect(),
            streams,
            chapters: Vec::new(),
        }
    }

    #[test]
    fn flags_site_stamps_and_offers_standard_names() {
        let info = file(
            vec![
                video("www.1TamilMV.io"),
                audio("www.1TamilMV.io - [DD5.1 - 384Kbps]"),
            ],
            Some("Downloaded from site.example.com"),
        );
        let issues = review(&info);
        assert_eq!(issues.len(), 3);
        assert_eq!(issues[0].target, Target::Container);
        assert_eq!(issues[0].standard, None);
        assert_eq!(issues[1].reason, Reason::WebAddress);
        assert_eq!(issues[1].standard.as_deref(), Some("1080p H.264"));
        assert_eq!(issues[2].standard.as_deref(), Some("5.1 AC-3 384 kbps"));
    }

    #[test]
    fn accepts_names_that_describe_the_stream() {
        for title in ["", "1080p H.264", "x264 1080p", "1920x800", "AVC"] {
            assert!(
                review(&file(vec![video(title)], None)).is_empty(),
                "{title}"
            );
        }
        for title in [
            "AC-3 5.1 384 kbps",
            "5.1 AC-3 384 kbps",
            "DD5.1 - 384Kbps",
            "Tamil",
            "Tamil DD 5.1",
            "Surround 5.1",
            "[tam] AC3",
        ] {
            assert!(
                review(&file(vec![audio(title)], None)).is_empty(),
                "{title}"
            );
        }
        assert!(review(&file(vec![audio("Director’s Commentary")], None)).is_empty());
    }

    #[test]
    fn flags_names_that_contradict_or_wander() {
        for title in ["720p H.264", "HEVC", "Encoded by SomeGroup"] {
            let issues = review(&file(vec![video(title)], None));
            assert_eq!(issues.len(), 1, "{title}");
            assert_eq!(issues[0].reason, Reason::Unrelated);
        }
        for title in ["AAC 2.0", "Stereo", "Hindi", "AC3 5.1 640kbps", "SomeGroup"] {
            assert_eq!(review(&file(vec![audio(title)], None)).len(), 1, "{title}");
        }
    }

    #[test]
    fn web_addresses() {
        assert!(has_web_address("www.example.io"));
        assert!(has_web_address("[TamilSite.com] - DD5.1"));
        assert!(has_web_address("see https://x.y/z"));
        assert!(!has_web_address("H.264 5.1 2.0 DTS-HD"));
        assert!(!has_web_address("Mr. Robot"));
    }

    #[test]
    fn walks_folders_at_any_depth() {
        let dir = tempfile::tempdir().unwrap();
        let deep = dir.path().join("a/b/c");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::create_dir_all(dir.path().join(".hidden")).unwrap();
        for file in [
            "top.mkv",
            "a/one.mp4",
            "a/b/c/deep.mkv",
            "a/notes.txt",
            ".hidden/skip.mkv",
            "a/.skip.mkv",
        ] {
            std::fs::write(dir.path().join(file), []).unwrap();
        }
        let videos = video_files(dir.path(), &AtomicBool::new(false));
        assert!(videos.unread.is_empty());
        let found: Vec<_> = videos
            .files
            .into_iter()
            .map(|p| {
                p.strip_prefix(dir.path())
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(found, ["a/b/c/deep.mkv", "a/one.mp4", "top.mkv"]);
    }

    #[cfg(unix)]
    #[test]
    fn says_which_folders_it_could_not_read() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let locked = dir.path().join("Locked");
        std::fs::create_dir_all(&locked).unwrap();
        std::fs::write(locked.join("inside.mkv"), []).unwrap();
        std::fs::write(dir.path().join("tidy.mkv"), []).unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        // Someone who can read anything (root) has nothing to report.
        let readable = std::fs::read_dir(&locked).is_ok();
        let videos = video_files(dir.path(), &AtomicBool::new(false));
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        if readable {
            return;
        }
        assert_eq!(videos.files, [dir.path().join("tidy.mkv")]);
        assert_eq!(
            videos.unread,
            [Unread {
                path: locked,
                folder: true
            }]
        );
    }

    #[test]
    fn audio_names_lead_with_the_layout() {
        let mono = Stream {
            channels: Some(1),
            ..stream(1, StreamKind::Audio, "aac", "")
        };
        assert_eq!(standard_name(&mono).as_deref(), Some("Mono AAC"));
        // A standard name always describes its own stream.
        let named = Stream {
            title: standard_name(&audio("")),
            ..audio("")
        };
        assert!(review(&file(vec![named], None)).is_empty());
    }
}

/// How flagged names are put right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fix {
    /// Replace each name with the standard one, or remove it when there is none.
    Standard,
    /// Remove the names.
    Remove,
}

/// The file edits that apply `fix` to the given issues.
pub fn edits<'a>(issues: impl IntoIterator<Item = &'a Issue>, fix: Fix) -> crate::remux::Edits {
    let mut edits = crate::remux::Edits::default();
    for issue in issues {
        let title = match fix {
            Fix::Standard => issue.standard.clone(),
            Fix::Remove => None,
        };
        match issue.target {
            Target::Container => edits.container_title = Some(title),
            Target::Stream(index) => {
                edits.titles.insert(index, title);
            }
        }
    }
    edits
}
