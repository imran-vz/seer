//! The one place values are turned into text for people.

use chrono::{DateTime, Local};
use std::time::SystemTime;

/// File size in decimal units, as Finder shows it.
pub fn size(bytes: u64) -> String {
    const KB: f64 = 1e3;
    const MB: f64 = 1e6;
    const GB: f64 = 1e9;
    const TB: f64 = 1e12;
    let b = bytes as f64;
    match bytes {
        0 => "Zero bytes".into(),
        1 => "1 byte".into(),
        _ if b < KB => format!("{bytes} bytes"),
        _ if b < MB => format!("{:.0} KB", b / KB),
        _ if b < GB => format!("{:.1} MB", b / MB),
        _ if b < TB => format!("{:.2} GB", b / GB),
        _ => format!("{:.2} TB", b / TB),
    }
}

/// A count and its noun: "1 item", "4 items". The plural adds an "s".
pub fn count(n: usize, noun: &str) -> String {
    match n {
        1 => format!("1 {noun}"),
        n => format!("{n} {noun}s"),
    }
}

/// A fraction from 0 to 1 as a whole percentage: "44%". Never shows 100%
/// before the work is complete.
pub fn percent(fraction: f32) -> String {
    let fraction = fraction.clamp(0., 1.);
    let whole = (fraction * 100.).round() as u32;
    let whole = if whole == 100 && fraction < 1. {
        99
    } else {
        whole
    };
    format!("{whole}%")
}

/// Bit rate in bits per second.
pub fn bitrate(bps: u64) -> String {
    let b = bps as f64;
    if b >= 1e6 {
        format!("{:.1} Mbps", b / 1e6)
    } else if b >= 1e3 {
        format!("{:.0} kbps", b / 1e3)
    } else {
        format!("{bps} bps")
    }
}

/// Duration as h:mm:ss, or m:ss under an hour.
pub fn duration(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    let (h, m, s) = (total / 3600, total / 60 % 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// A point on a timeline: seconds with tenths when the span is under a
/// minute, otherwise a clock time.
pub fn timestamp(seconds: f64, span: f64) -> String {
    if span >= 60.0 {
        return duration(seconds);
    }
    let text = format!("{seconds:.1}");
    format!("{}s", text.trim_end_matches(".0"))
}

pub fn frame_rate(fps: f64) -> String {
    let text = format!("{fps:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!("{text} fps")
}

pub fn sample_rate(hz: u32) -> String {
    let khz = hz as f64 / 1000.0;
    if khz.fract() == 0.0 {
        format!("{khz:.0} kHz")
    } else {
        format!("{khz:.1} kHz")
    }
}

/// "Today at 14:32", "Yesterday at 09:10", otherwise "19 Nov 2026 at 14:32".
pub fn date(time: SystemTime) -> String {
    date_relative_to(time.into(), Local::now())
}

fn date_relative_to(t: DateTime<Local>, now: DateTime<Local>) -> String {
    let days = (now.date_naive() - t.date_naive()).num_days();
    let clock = t.format("%H:%M");
    match days {
        0 => format!("Today at {clock}"),
        1 => format!("Yesterday at {clock}"),
        _ => format!("{} at {clock}", t.format("%-d %b %Y")),
    }
}

/// Display name for an FFmpeg codec id, e.g. "hevc" becomes "HEVC".
pub fn codec(name: &str) -> String {
    let known = match name {
        "h264" => "H.264",
        "hevc" => "HEVC",
        "vvc" => "VVC",
        "av1" => "AV1",
        "vp8" => "VP8",
        "vp9" => "VP9",
        "mpeg4" => "MPEG-4",
        "mpeg2video" => "MPEG-2",
        "mpeg1video" => "MPEG-1",
        "prores" => "ProRes",
        "dnxhd" => "DNxHD",
        "mjpeg" => "MJPEG",
        "vc1" => "VC-1",
        "wmv3" => "WMV",
        "theora" => "Theora",
        "aac" => "AAC",
        "ac3" => "AC-3",
        "eac3" => "E-AC-3",
        "dts" => "DTS",
        "truehd" => "TrueHD",
        "mlp" => "MLP",
        "flac" => "FLAC",
        "alac" => "ALAC",
        "opus" => "Opus",
        "vorbis" => "Vorbis",
        "mp3" => "MP3",
        "mp2" => "MP2",
        "wmav2" => "WMA",
        "subrip" => "SRT",
        "ass" | "ssa" => "ASS",
        "webvtt" => "WebVTT",
        "mov_text" => "Timed Text",
        "hdmv_pgs_subtitle" => "PGS",
        "dvd_subtitle" => "VobSub",
        "dvb_subtitle" => "DVB",
        "ttf" => "TrueType Font",
        "otf" => "OpenType Font",
        "png" => "PNG",
        other if other.starts_with("pcm_") => "PCM",
        other => return other.to_uppercase(),
    };
    known.to_owned()
}

/// Display name for a container, from FFmpeg's demuxer name and its
/// description: "FLAC", not "raw FLAC"; "Matroska / WebM", not
/// "matroska,webm". Falls back to the description without FFmpeg's
/// "raw " prefix and parenthetical.
pub fn container(name: &str, description: &str) -> String {
    let known = match name {
        "flac" => "FLAC",
        "mp3" => "MP3",
        "aac" => "AAC",
        "ac3" => "AC-3",
        "eac3" => "E-AC-3",
        "wav" => "WAV",
        "avi" => "AVI",
        "ogg" => "Ogg",
        "matroska,webm" => "Matroska / WebM",
        "mpegts" => "MPEG-TS",
        "asf" => "ASF",
        "flv" => "FLV",
        _ => {
            let text = description.strip_prefix("raw ").unwrap_or(description);
            let text = match text.find(" (") {
                Some(at) => &text[..at],
                None => text,
            };
            return match text.trim() {
                "" => name.split(',').next().unwrap_or(name).to_uppercase(),
                text => text.to_owned(),
            };
        }
    };
    known.to_owned()
}

/// English name for an ISO 639 language code, or the code itself.
pub fn language(code: &str) -> String {
    let name = match code.to_ascii_lowercase().as_str() {
        "eng" | "en" => "English",
        "fra" | "fre" | "fr" => "French",
        "deu" | "ger" | "de" => "German",
        "spa" | "es" => "Spanish",
        "ita" | "it" => "Italian",
        "por" | "pt" => "Portuguese",
        "nld" | "dut" | "nl" => "Dutch",
        "rus" | "ru" => "Russian",
        "jpn" | "ja" => "Japanese",
        "kor" | "ko" => "Korean",
        "zho" | "chi" | "zh" => "Chinese",
        "ara" | "ar" => "Arabic",
        "hin" | "hi" => "Hindi",
        "tam" | "ta" => "Tamil",
        "tel" | "te" => "Telugu",
        "mal" | "ml" => "Malayalam",
        "kan" | "kn" => "Kannada",
        "ben" | "bn" => "Bengali",
        "urd" | "ur" => "Urdu",
        "tur" | "tr" => "Turkish",
        "pol" | "pl" => "Polish",
        "swe" | "sv" => "Swedish",
        "nor" | "nob" | "no" => "Norwegian",
        "dan" | "da" => "Danish",
        "fin" | "fi" => "Finnish",
        "ces" | "cze" | "cs" => "Czech",
        "ell" | "gre" | "el" => "Greek",
        "heb" | "he" => "Hebrew",
        "hun" | "hu" => "Hungarian",
        "ron" | "rum" | "ro" => "Romanian",
        "tha" | "th" => "Thai",
        "vie" | "vi" => "Vietnamese",
        "ind" | "id" => "Indonesian",
        "ukr" | "uk" => "Ukrainian",
        _ => return code.to_owned(),
    };
    name.to_owned()
}

/// "Mono", "Stereo", "5.1", "7.1", otherwise a channel count.
pub fn channels(count: u32) -> String {
    match count {
        1 => "Mono".into(),
        2 => "Stereo".into(),
        6 => "5.1".into(),
        8 => "7.1".into(),
        n => format!("{n} ch"),
    }
}

/// How big a video stream's picture is, in the fewest words: its class
/// ("1080p", "4K"), or its pixel size below the classes ("176×144").
pub fn video_size(width: Option<u32>, height: Option<u32>) -> Option<String> {
    let (w, h) = (width?, height?);
    Some(match resolution_class(w, h) {
        Some(class) => class.to_owned(),
        None => format!("{w}×{h}"),
    })
}

/// A video stream as Seer names it everywhere (the list, Clean Up's
/// proposed names, the inspector): its size, then its codec. "1080p H.264".
pub fn video_name(codec: &str, width: Option<u32>, height: Option<u32>) -> String {
    let codec = self::codec(codec);
    match video_size(width, height) {
        Some(size) => format!("{size} {codec}"),
        None => codec,
    }
}

/// An audio stream as Seer names it everywhere: its channel layout, then
/// its codec, in the order a video leads with its size. "Mono AAC", "5.1
/// E-AC-3".
pub fn audio_name(codec: &str, channels: Option<u32>) -> String {
    let codec = self::codec(codec);
    match channels {
        Some(count) => format!("{} {codec}", self::channels(count)),
        None => codec,
    }
}

pub fn resolution(width: u32, height: u32) -> String {
    format!("{width} × {height}")
}

/// "1080p", "4K" and the like, down to 240p. `None` below that.
pub fn resolution_class(width: u32, height: u32) -> Option<&'static str> {
    // Width decides for wide (cropped) content, height for the rest.
    Some(match (width, height) {
        (w, h) if w >= 7600 || h >= 4300 => "8K",
        (w, h) if w >= 3800 || h >= 2100 => "4K",
        (w, h) if w >= 2500 || h >= 1400 => "1440p",
        (w, h) if w >= 1900 || h >= 1060 => "1080p",
        (w, h) if w >= 1260 || h >= 700 => "720p",
        (_, h) if h >= 560 => "576p",
        (_, h) if h >= 470 => "480p",
        (_, h) if h >= 340 => "360p",
        (_, h) if h >= 220 => "240p",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn sizes() {
        assert_eq!(size(0), "Zero bytes");
        assert_eq!(size(999), "999 bytes");
        assert_eq!(size(12_400), "12 KB");
        assert_eq!(size(4_210_000), "4.2 MB");
        assert_eq!(size(1_370_000_000), "1.37 GB");
    }

    #[test]
    fn counts_and_percentages() {
        assert_eq!(count(1, "item"), "1 item");
        assert_eq!(count(0, "file"), "0 files");
        assert_eq!(count(4, "item"), "4 items");
        assert_eq!(percent(0.44), "44%");
        assert_eq!(percent(0.996), "99%");
        assert_eq!(percent(1.), "100%");
        assert_eq!(percent(-1.), "0%");
    }

    #[test]
    fn rates_and_durations() {
        assert_eq!(bitrate(4_230_000), "4.2 Mbps");
        assert_eq!(bitrate(192_000), "192 kbps");
        assert_eq!(duration(187.2), "3:07");
        assert_eq!(duration(5025.0), "1:23:45");
        assert_eq!(timestamp(20.0, 40.0), "20s");
        assert_eq!(timestamp(0.5, 40.0), "0.5s");
        assert_eq!(timestamp(75.0, 400.0), "1:15");
        assert_eq!(frame_rate(24.0), "24 fps");
        assert_eq!(frame_rate(23.976_023), "23.976 fps");
        assert_eq!(frame_rate(29.97), "29.97 fps");
        assert_eq!(sample_rate(48_000), "48 kHz");
        assert_eq!(sample_rate(44_100), "44.1 kHz");
    }

    #[test]
    fn dates() {
        let now = Local.with_ymd_and_hms(2026, 11, 19, 18, 0, 0).unwrap();
        let at = |d, h| Local.with_ymd_and_hms(2026, 11, d, h, 5, 0).unwrap();
        assert_eq!(date_relative_to(at(19, 9), now), "Today at 09:05");
        assert_eq!(date_relative_to(at(18, 23), now), "Yesterday at 23:05");
        assert_eq!(date_relative_to(at(2, 14), now), "2 Nov 2026 at 14:05");
    }

    #[test]
    fn codecs_and_channels() {
        assert_eq!(codec("hevc"), "HEVC");
        assert_eq!(codec("pcm_s24le"), "PCM");
        assert_eq!(codec("cook"), "COOK");
        assert_eq!(channels(6), "5.1");
        assert_eq!(channels(3), "3 ch");
    }

    #[test]
    fn containers() {
        assert_eq!(container("flac", "raw FLAC"), "FLAC");
        assert_eq!(
            container("matroska,webm", "Matroska / WebM"),
            "Matroska / WebM"
        );
        assert_eq!(
            container("mov,mp4,m4a,3gp,3g2,mj2", "QuickTime / MOV"),
            "QuickTime / MOV"
        );
        assert_eq!(container("dts", "raw DTS"), "DTS");
        assert_eq!(container("wv", "WavPack (lossless audio)"), "WavPack");
        assert_eq!(container("xyz", ""), "XYZ");
    }

    #[test]
    fn resolution_classes() {
        assert_eq!(resolution_class(3840, 1600), Some("4K"));
        assert_eq!(resolution_class(1920, 800), Some("1080p"));
        assert_eq!(resolution_class(1280, 720), Some("720p"));
        assert_eq!(resolution_class(640, 360), Some("360p"));
        assert_eq!(resolution_class(320, 240), Some("240p"));
        assert_eq!(resolution_class(176, 144), None);
    }

    /// One order for media names everywhere: what the stream is like
    /// (size, layout) first, then the codec (decision 65).
    #[test]
    fn media_names_lead_with_size_and_layout() {
        assert_eq!(video_size(Some(1920), Some(1080)).as_deref(), Some("1080p"));
        assert_eq!(video_size(Some(176), Some(144)).as_deref(), Some("176×144"));
        assert_eq!(video_size(None, Some(1080)), None);
        assert_eq!(video_name("h264", Some(1920), Some(1080)), "1080p H.264");
        assert_eq!(video_name("mpeg2video", None, None), "MPEG-2");
        assert_eq!(audio_name("aac", Some(1)), "Mono AAC");
        assert_eq!(audio_name("eac3", Some(6)), "5.1 E-AC-3");
        assert_eq!(audio_name("opus", None), "Opus");
    }
}
