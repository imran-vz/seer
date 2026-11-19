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

pub fn resolution(width: u32, height: u32) -> String {
    format!("{width} × {height}")
}

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
    fn resolution_classes() {
        assert_eq!(resolution_class(3840, 1600), Some("4K"));
        assert_eq!(resolution_class(1920, 800), Some("1080p"));
        assert_eq!(resolution_class(1280, 720), Some("720p"));
        assert_eq!(resolution_class(320, 240), None);
    }
}
