//! Suggestions for which streams a file could lose.
//!
//! These only propose. Nothing here changes a file, and every suggestion
//! says why it was made so the person can judge it.

use crate::format;
use crate::probe::{MediaInfo, Stream, StreamKind};

/// Streams that duplicate a better one: every video but the best, and for
/// each audio language every track but the best. Cover art, subtitles and
/// attachments are never suggested.
///
/// Audio is compared within a language, never across languages, so this
/// cannot remove the only track someone understands.
pub fn lower_quality(info: &MediaInfo) -> Vec<usize> {
    let mut remove = Vec::new();

    let videos: Vec<&Stream> = info
        .streams_of(StreamKind::Video)
        .filter(|s| !s.flags.attached_picture)
        .collect();
    if let Some(best) = videos.iter().max_by_key(|s| video_rank(s)) {
        remove.extend(
            videos
                .iter()
                .filter(|s| s.index != best.index)
                .map(|s| s.index),
        );
    }

    let audio: Vec<&Stream> = info.streams_of(StreamKind::Audio).collect();
    for language in audio_languages(info).into_iter().map(Some).chain([None]) {
        let group: Vec<&&Stream> = audio
            .iter()
            .filter(|s| language_name(s) == language)
            .collect();
        if let Some(best) = group.iter().max_by_key(|s| audio_rank(s)) {
            remove.extend(
                group
                    .iter()
                    .filter(|s| s.index != best.index)
                    .map(|s| s.index),
            );
        }
    }

    remove.sort_unstable();
    remove
}

/// Bigger is better: picture size, then bit depth, then bit rate.
fn video_rank(stream: &Stream) -> (u64, u32, u64) {
    let pixels = stream.width.unwrap_or(0) as u64 * stream.height.unwrap_or(0) as u64;
    (
        pixels,
        stream.bit_depth.unwrap_or(0),
        stream.bit_rate.unwrap_or(0),
    )
}

/// Bigger is better. A main track beats a commentary or described one,
/// lossless beats lossy, then more channels, then a higher bit rate. Earlier
/// streams win ties.
fn audio_rank(stream: &Stream) -> (bool, bool, u32, u64, std::cmp::Reverse<usize>) {
    let main =
        !(stream.flags.commentary || stream.flags.visual_impaired || stream.flags.hearing_impaired);
    (
        main,
        is_lossless(stream),
        stream.channels.unwrap_or(0),
        stream.bit_rate.unwrap_or(0),
        std::cmp::Reverse(stream.index),
    )
}

fn is_lossless(stream: &Stream) -> bool {
    let profile = stream.profile.as_deref().unwrap_or("");
    match stream.codec.as_str() {
        "truehd" | "mlp" | "flac" | "alac" | "wavpack" | "ape" | "tta" => true,
        // Only the Master Audio flavour of DTS is lossless.
        "dts" => profile.contains("MA"),
        codec => codec.starts_with("pcm_"),
    }
}

/// Why a language is thought to be the one the film was made in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evidence {
    /// A track carries the container's "original language" flag.
    MarkedOriginal,
    /// Every other language is flagged as a dub.
    OthersAreDubs,
    /// It is the default track. A convention, not a fact.
    DefaultTrack,
    /// It is the first track. The weakest hint.
    FirstTrack,
}

impl Evidence {
    /// True when the file itself states it, rather than us inferring it.
    pub fn is_stated(self) -> bool {
        matches!(self, Evidence::MarkedOriginal | Evidence::OthersAreDubs)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Original {
    /// Display name, e.g. "Tamil".
    pub language: String,
    pub evidence: Evidence,
}

/// Distinct languages of the audio tracks, in stream order, by display name.
pub fn audio_languages(info: &MediaInfo) -> Vec<String> {
    let mut languages: Vec<String> = Vec::new();
    for name in info.streams_of(StreamKind::Audio).filter_map(language_name) {
        if !languages.contains(&name) {
            languages.push(name);
        }
    }
    languages
}

/// Best guess at the language the film was made in. `None` unless the file
/// has audio in at least two languages, since otherwise there is nothing to choose.
pub fn original_language(info: &MediaInfo) -> Option<Original> {
    if audio_languages(info).len() < 2 {
        return None;
    }
    let tagged: Vec<&Stream> = info
        .streams_of(StreamKind::Audio)
        .filter(|s| language_name(s).is_some())
        .collect();
    let found = |stream: Option<&&Stream>, evidence| {
        stream
            .and_then(|s| language_name(s))
            .map(|language| Original { language, evidence })
    };

    if let Some(original) = found(
        tagged.iter().find(|s| s.flags.original),
        Evidence::MarkedOriginal,
    ) {
        return Some(original);
    }
    // If dubs are flagged and exactly one language is free of them, that is it.
    let mut undubbed: Vec<String> = Vec::new();
    for name in tagged
        .iter()
        .filter(|s| !s.flags.dub)
        .filter_map(|s| language_name(s))
    {
        if !undubbed.contains(&name) {
            undubbed.push(name);
        }
    }
    if tagged.iter().any(|s| s.flags.dub) && undubbed.len() == 1 {
        return Some(Original {
            language: undubbed.remove(0),
            evidence: Evidence::OthersAreDubs,
        });
    }
    found(
        tagged.iter().find(|s| s.flags.default),
        Evidence::DefaultTrack,
    )
    .or_else(|| found(tagged.first(), Evidence::FirstTrack))
}

/// Audio tracks in a language other than `keep`. Tracks with no language tag
/// are left alone: we cannot say what they are.
pub fn other_languages(info: &MediaInfo, keep: &str) -> Vec<usize> {
    info.streams_of(StreamKind::Audio)
        .filter(|s| language_name(s).is_some_and(|name| name != keep))
        .map(|s| s.index)
        .collect()
}

/// Display name of a stream's language, so "fre" and "fra" count as one.
fn language_name(stream: &Stream) -> Option<String> {
    stream.language.as_deref().map(format::language)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::StreamFlags;

    fn stream(index: usize, kind: StreamKind, codec: &str) -> Stream {
        Stream {
            index,
            kind,
            codec: codec.into(),
            codec_description: None,
            profile: None,
            language: None,
            title: None,
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

    fn video(index: usize, width: u32, height: u32) -> Stream {
        Stream {
            width: Some(width),
            height: Some(height),
            ..stream(index, StreamKind::Video, "h264")
        }
    }

    fn audio(index: usize, codec: &str, language: &str, channels: u32, bit_rate: u64) -> Stream {
        Stream {
            language: Some(language.into()),
            channels: Some(channels),
            bit_rate: Some(bit_rate),
            ..stream(index, StreamKind::Audio, codec)
        }
    }

    fn file(streams: Vec<Stream>) -> MediaInfo {
        MediaInfo {
            container: "matroska".into(),
            container_description: "Matroska".into(),
            duration: 60.0,
            bit_rate: 0,
            tags: Vec::new(),
            streams,
            chapters: Vec::new(),
        }
    }

    #[test]
    fn keeps_best_audio_per_language() {
        let info = file(vec![
            video(0, 1920, 800),
            audio(1, "ac3", "tam", 6, 384_000),
            audio(2, "aac", "tam", 2, 128_000),
            audio(3, "aac", "hin", 2, 128_000),
            stream(4, StreamKind::Subtitle, "subrip"),
        ]);
        // The stereo Tamil copy goes; the only Hindi track and the subtitles stay.
        assert_eq!(lower_quality(&info), [2]);
    }

    #[test]
    fn prefers_lossless_then_channels_and_never_commentary() {
        let mut commentary = audio(3, "truehd", "eng", 8, 4_000_000);
        commentary.flags.commentary = true;
        let mut dts_ma = audio(2, "dts", "eng", 6, 3_000_000);
        dts_ma.profile = Some("DTS-HD MA".into());
        let info = file(vec![
            video(0, 3840, 2160),
            audio(1, "eac3", "eng", 8, 1_500_000),
            dts_ma,
            commentary,
        ]);
        assert_eq!(lower_quality(&info), [1, 3]);
    }

    #[test]
    fn keeps_largest_video_and_ignores_cover_art() {
        let mut cover = video(2, 4000, 4000);
        cover.flags.attached_picture = true;
        let info = file(vec![video(0, 1280, 720), video(1, 1920, 1080), cover]);
        assert_eq!(lower_quality(&info), [0]);
    }

    #[test]
    fn nothing_to_suggest_for_a_plain_file() {
        let info = file(vec![
            video(0, 1920, 1080),
            audio(1, "aac", "eng", 2, 128_000),
        ]);
        assert!(lower_quality(&info).is_empty());
        assert_eq!(original_language(&info), None);
    }

    #[test]
    fn original_language_by_strength_of_evidence() {
        let base = || {
            vec![
                video(0, 1920, 1080),
                audio(1, "aac", "hin", 2, 128_000),
                audio(2, "aac", "tam", 2, 128_000),
                audio(3, "aac", "tel", 2, 128_000),
            ]
        };
        let guess = |streams| original_language(&file(streams)).unwrap();

        assert_eq!(
            guess(base()),
            Original {
                language: "Hindi".into(),
                evidence: Evidence::FirstTrack
            }
        );

        let mut streams = base();
        streams[2].flags.default = true;
        assert_eq!(guess(streams.clone()).evidence, Evidence::DefaultTrack);
        assert_eq!(guess(streams.clone()).language, "Tamil");
        assert!(!Evidence::DefaultTrack.is_stated());

        // Dub flags outrank the default flag.
        streams[1].flags.dub = true;
        streams[2].flags.dub = true;
        assert_eq!(
            guess(streams.clone()),
            Original {
                language: "Telugu".into(),
                evidence: Evidence::OthersAreDubs
            }
        );

        // An explicit original flag outranks everything.
        streams[1].flags.original = true;
        assert_eq!(
            guess(streams),
            Original {
                language: "Hindi".into(),
                evidence: Evidence::MarkedOriginal
            }
        );
    }

    #[test]
    fn removes_other_languages_but_not_untagged_tracks() {
        let untagged = Stream {
            language: None,
            ..audio(4, "aac", "x", 2, 0)
        };
        let info = file(vec![
            video(0, 1920, 1080),
            audio(1, "aac", "fre", 2, 128_000),
            audio(2, "ac3", "fra", 6, 384_000),
            audio(3, "aac", "eng", 2, 128_000),
            untagged,
        ]);
        assert_eq!(audio_languages(&info), ["French", "English"]);
        assert_eq!(other_languages(&info, "French"), [3]);
        assert_eq!(other_languages(&info, "English"), [1, 2]);
    }
}
